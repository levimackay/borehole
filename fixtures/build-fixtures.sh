#!/usr/bin/env bash
# Builds every fixture repository under fixtures/ from scratch, with real
# git commits and deterministic timestamps/authors so history-dependent
# tests (rename tracking, temporal coupling, introduced-commit) are
# reproducible. Safe to re-run: each fixture directory is wiped and rebuilt.
#
# Usage: fixtures/build-fixtures.sh
set -euo pipefail

FIXTURES_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export GIT_AUTHOR_NAME="Fixture Author"
export GIT_AUTHOR_EMAIL="fixtures@borehole.test"
export GIT_COMMITTER_NAME="Fixture Author"
export GIT_COMMITTER_EMAIL="fixtures@borehole.test"

# $1 = fixture name, $2 = ISO date for this commit's author/committer date
commit_at() {
  local msg="$1" date="$2"
  GIT_AUTHOR_DATE="$date" GIT_COMMITTER_DATE="$date" git commit -q -m "$msg"
}

new_fixture() {
  local name="$1"
  local dir="$FIXTURES_DIR/$name"
  rm -rf "$dir"
  mkdir -p "$dir"
  cd "$dir"
  git init -q -b main
}

# ---------------------------------------------------------------------------
echo "simple-ts"
new_fixture simple-ts
cat > math.ts <<'EOF'
export function add(a: number, b: number): number {
  return a + b;
}
EOF
git add -A && commit_at "Add math helper" "2026-01-01T10:00:00"
cat > NOTES.md <<'EOF'
# simple-ts
Single-file baseline: one exported function, no imports, no dependents.
Sanity check that indexing a trivial repo doesn't crash and finds `add`.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-01T10:05:00"

# ---------------------------------------------------------------------------
echo "nested-deps"
new_fixture nested-deps
cat > c.ts <<'EOF'
export function base(): string {
  return "base";
}
EOF
cat > b.ts <<'EOF'
import { base } from "./c";

export function middle(): string {
  return base() + "-middle";
}
EOF
cat > a.ts <<'EOF'
import { middle } from "./b";

export function top(): string {
  return middle() + "-top";
}
EOF
git add -A && commit_at "Three-level import chain: a -> b -> c" "2026-01-02T10:00:00"
cat > NOTES.md <<'EOF'
# nested-deps
a.ts imports b.ts imports c.ts. Tests caller/callee traversal depth:
callees of `top` (depth 1) should reach `middle`; depth 2 should reach `base`.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-02T10:05:00"

# ---------------------------------------------------------------------------
echo "cycle"
new_fixture cycle
cat > even.ts <<'EOF'
import { isOdd } from "./odd";

export function isEven(n: number): boolean {
  if (n === 0) return true;
  return isOdd(n - 1);
}
EOF
cat > odd.ts <<'EOF'
import { isEven } from "./even";

export function isOdd(n: number): boolean {
  if (n === 0) return false;
  return isEven(n - 1);
}
EOF
git add -A && commit_at "Mutually recursive modules" "2026-01-03T10:00:00"
cat > NOTES.md <<'EOF'
# cycle
even.ts and odd.ts import each other. Tests that graph traversal
(callers/callees expansion) is cycle-safe: a bounded-depth BFS must
terminate and must not re-add a visited symbol as a duplicate node.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-03T10:05:00"

# ---------------------------------------------------------------------------
echo "inheritance"
new_fixture inheritance
cat > shapes.py <<'EOF'
from abc import ABC, abstractmethod


class Shape(ABC):
    @abstractmethod
    def area(self) -> float:
        ...


class Polygon(Shape):
    def area(self) -> float:
        raise NotImplementedError


class Square(Polygon):
    def __init__(self, side: float):
        self.side = side

    def area(self) -> float:
        return self.side * self.side
EOF
git add -A && commit_at "Three-level Python class hierarchy" "2026-01-04T10:00:00"
cat > NOTES.md <<'EOF'
# inheritance
Shape (ABC) <- Polygon <- Square, three levels deep. Tests subclass
discovery: subclasses_of(Shape) should find both Polygon and Square;
subclasses_of(Polygon) should find only Square.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-04T10:05:00"

# ---------------------------------------------------------------------------
echo "rename-history"
new_fixture rename-history
cat > session.ts <<'EOF'
export class SessionService {
  renew(): void {
    // v1
  }
}
EOF
git add -A && commit_at "Add SessionService" "2026-01-05T10:00:00"
cat > session.ts <<'EOF'
export class SessionService {
  renew(): void {
    // v2: added logging
    console.log("renewing session");
  }
}
EOF
git add -A && commit_at "Log session renewal" "2026-01-05T11:00:00"
git mv session.ts auth-session.ts
commit_at "Rename session.ts to auth-session.ts" "2026-01-05T12:00:00"
cat >> auth-session.ts <<'EOF'

export function isExpired(): boolean {
  return false;
}
EOF
git add -A && commit_at "Add isExpired helper after rename" "2026-01-05T13:00:00"
cat > NOTES.md <<'EOF'
# rename-history
session.ts is edited twice, renamed to auth-session.ts, then edited again.
Tests that file_history on "auth-session.ts" follows the rename and
returns all 4 commits, and that symbol_history for SessionService.renew
(defined before the rename) still resolves across it.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-05T13:05:00"

# ---------------------------------------------------------------------------
echo "deleted-symbol"
new_fixture deleted-symbol
cat > legacy.ts <<'EOF'
export function oldHelper(): string {
  return "deprecated";
}

export function stillUsed(): string {
  return oldHelper() + "!";
}
EOF
git add -A && commit_at "Add legacy helpers" "2026-01-06T10:00:00"
cat > legacy.ts <<'EOF'
export function stillUsed(): string {
  return "no longer calls oldHelper";
}
EOF
git add -A && commit_at "Remove oldHelper, it had no other callers" "2026-01-06T11:00:00"
cat > NOTES.md <<'EOF'
# deleted-symbol
oldHelper is defined, referenced once, then deleted in a later commit.
Tests that a query for oldHelper after the deletion returns "not found"
rather than stale data, while its history is still visible via
file_history on legacy.ts.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-06T11:05:00"

# ---------------------------------------------------------------------------
echo "monorepo"
new_fixture monorepo
mkdir -p packages/core packages/app
cat > package.json <<'EOF'
{
  "name": "monorepo-fixture",
  "private": true,
  "workspaces": ["packages/*"]
}
EOF
cat > packages/core/index.ts <<'EOF'
export function coreUtil(): string {
  return "core";
}
EOF
cat > packages/app/index.ts <<'EOF'
import { coreUtil } from "../core/index";

export function appEntry(): string {
  return coreUtil() + "-app";
}
EOF
git add -A && commit_at "Two-package npm workspace" "2026-01-07T10:00:00"
cat > NOTES.md <<'EOF'
# monorepo
npm workspaces with packages/core and packages/app, app depends on core
via a relative cross-package import. Tests workspace/package detection
and that cross-package references resolve like any other import.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-07T10:05:00"

# ---------------------------------------------------------------------------
echo "generated-files"
new_fixture generated-files
cat > real.ts <<'EOF'
export function handWritten(): string {
  return "real";
}
EOF
cat > schema.pb.go <<'EOF'
// AUTO-GENERATED. DO NOT EDIT.
package schema

func GeneratedGetter() string {
	return "generated"
}
EOF
git add -A && commit_at "One real file, one generated file" "2026-01-08T10:00:00"
cat > NOTES.md <<'EOF'
# generated-files
schema.pb.go matches the default generated_path_globs (**/*.pb.go).
Tests that generated files are still indexed for navigation but excluded
from "before you change this" reading lists and temporal coupling.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-08T10:05:00"

# ---------------------------------------------------------------------------
echo "ambiguous-refs"
new_fixture ambiguous-refs
mkdir -p billing shipping
cat > billing/handler.ts <<'EOF'
export class Handler {
  process(): string {
    return "billing";
  }
}
EOF
cat > shipping/handler.ts <<'EOF'
export class Handler {
  process(): string {
    return "shipping";
  }
}
EOF
cat > caller.ts <<'EOF'
// No import — calls `process()` on some value of unknown origin. A
// name-only resolver has no scope information to disambiguate which
// Handler.process this refers to.
declare function getHandler(): any;
const h = getHandler();
h.process();
EOF
git add -A && commit_at "Two unrelated same-named classes plus an ambiguous call" "2026-01-09T10:00:00"
cat > NOTES.md <<'EOF'
# ambiguous-refs
billing/Handler.process and shipping/Handler.process share a name with
no import connecting caller.ts to either. Tests that the resolver
refuses to guess: the reference must resolve to `to_symbol: None` at
Confidence::Low, never silently picked as one of the two candidates.
EOF
git add -A && commit_at "Add fixture notes" "2026-01-09T10:05:00"

# ---------------------------------------------------------------------------
echo "rust-basic"
new_fixture rust-basic
mkdir -p src
cat > Cargo.toml <<'EOF'
[package]
name = "rust-basic-fixture"
version = "0.1.0"
edition = "2021"

# Standalone [workspace] table so Cargo doesn't try to fold this fixture
# into the parent Borehole workspace when it's nested under fixtures/.
[workspace]
EOF
cat > src/lib.rs <<'EOF'
pub struct Counter {
    value: i64,
}

impl Counter {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn increment(&mut self) -> i64 {
        self.value += 1;
        self.value
    }
}

pub fn make_and_run() -> i64 {
    let mut c = Counter::new();
    c.increment();
    c.increment()
}
EOF
git add -A && commit_at "Minimal Rust crate with a struct and impl" "2026-01-10T10:00:00"
cat > NOTES.md <<'EOF'
# rust-basic
Struct + impl block + a free function calling both — cross-language
smoke coverage for the Rust extractor (methods qualified as
Counter::new / Counter::increment, a Call reference from make_and_run).
EOF
git add -A && commit_at "Add fixture notes" "2026-01-10T10:05:00"

# ---------------------------------------------------------------------------
echo "go-basic"
new_fixture go-basic
cat > go.mod <<'EOF'
module gobasicfixture

go 1.22
EOF
cat > main.go <<'EOF'
package main

import "fmt"

type Greeter struct {
	Name string
}

func (g Greeter) Greet() string {
	return "Hello, " + g.Name
}

func main() {
	g := Greeter{Name: "Borehole"}
	fmt.Println(g.Greet())
}
EOF
git add -A && commit_at "Minimal Go module with a struct and method" "2026-01-11T10:00:00"
cat > NOTES.md <<'EOF'
# go-basic
A struct, a method with a value receiver, and main() calling it —
cross-language smoke coverage for the Go extractor (exported-ness via
capitalization: Greeter/Greet/Name are exported, main is not).
EOF
git add -A && commit_at "Add fixture notes" "2026-01-11T10:05:00"

echo "Done. Fixtures built under $FIXTURES_DIR"
