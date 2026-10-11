// Scroll reveal: elements marked .reveal fade and slide in as they come into view.
const reveal = new IntersectionObserver((entries) => {
  for (const entry of entries) {
    if (entry.isIntersecting) {
      entry.target.classList.add("in");
      reveal.unobserve(entry.target);
    }
  }
}, { rootMargin: "0px 0px -10% 0px" });
for (const element of document.querySelectorAll(".reveal")) reveal.observe(element);

// Copy buttons: <button data-copy="id"> copies the text of the element with that id.
for (const button of document.querySelectorAll("[data-copy]")) {
  button.addEventListener("click", async () => {
    const text = document.getElementById(button.dataset.copy).textContent;
    await navigator.clipboard.writeText(text);
    button.textContent = "Copied";
    setTimeout(() => { button.textContent = "Copy"; }, 1500);
  });
}

// The pricing page's billing toggle, which changes the period and nothing else.
const periods = document.querySelectorAll(".switch button");
for (const button of periods) {
  button.addEventListener("click", () => {
    for (const other of periods) other.setAttribute("aria-pressed", String(other === button));
    const annual = button.dataset.period === "annually";
    for (const per of document.querySelectorAll(".per")) per.textContent = annual ? "/ year" : "/ month";
  });
}
