for (const button of document.querySelectorAll("[data-copy]")) {
  button.addEventListener("click", async () => {
    const label = button.querySelector("span");
    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      button.classList.add("done");
      if (label) label.textContent = "Copied";
    } catch {
      if (label) label.textContent = "Select and copy";
    }
    setTimeout(() => {
      button.classList.remove("done");
      if (label) label.textContent = "Copy";
    }, 1800);
  });
}

if ("IntersectionObserver" in window && !matchMedia("(prefers-reduced-motion: reduce)").matches) {
  const shown = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        entry.target.classList.add("seen");
        shown.unobserve(entry.target);
      }
    }
  }, { rootMargin: "0px 0px -8% 0px" });
  for (const el of document.querySelectorAll(".home .block .heading, .home .tile, .home .split > *, .home .kin, .home .install-card, .home .release")) {
    el.classList.add("reveal");
    shown.observe(el);
  }
}

const links = [...document.querySelectorAll(".toc a")];
if (links.length) {
  const byId = new Map(links.map((a) => [decodeURIComponent(a.hash.slice(1)), a]));
  const watch = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const link = byId.get(entry.target.id);
      if (!link) continue;
      for (const a of links) a.classList.toggle("on", a === link);
    }
  }, { rootMargin: "-80px 0px -70% 0px" });
  for (const id of byId.keys()) {
    const heading = document.getElementById(id);
    if (heading) watch.observe(heading);
  }
  if (matchMedia("(max-width: 960px)").matches) document.querySelector(".toc details")?.removeAttribute("open");
}
