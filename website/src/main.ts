import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-sans/700.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/site.css";

// The hero's reveal is pure CSS (see src/styles/site.css: .bh-hero__* +
// @keyframes bh-reveal). The only client-side behavior here is triggering
// the below-the-fold diptych rows' cross-fade the first time each one
// enters the viewport, and never again.
const revealRows = document.querySelectorAll<HTMLElement>(".split--reveal");

if (revealRows.length > 0) {
  if ("IntersectionObserver" in window) {
    const observer = new IntersectionObserver(
      (entries, obs) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            entry.target.classList.add("is-visible");
            obs.unobserve(entry.target);
          }
        }
      },
      { threshold: 0.2 },
    );

    revealRows.forEach((row) => observer.observe(row));
  } else {
    // No IntersectionObserver support: show the content immediately
    // rather than leaving it permanently hidden.
    revealRows.forEach((row) => row.classList.add("is-visible"));
  }
}
