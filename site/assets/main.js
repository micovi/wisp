// Scenarios from wisp's eval. `completion` is what Qwen2.5-Coder produced; `output` is the
// fixture the model saw above the prompt, with `source` marking where the name came from.
const SCENARIOS = {
  kubectl: {
    output: [
      "$ kubectl get pods -n billing",
      "NAME                        READY   STATUS             RESTARTS",
      "api-7d9f8b6c4-x2kqp         0/1     CrashLoopBackOff   12",
      "worker-5c8d7f9b6-mn4lz      1/1     Running            0",
    ],
    source: "api-7d9f8b6c4-x2kqp",
    typed: "kubectl logs -n billing api",
    completion: "-7d9f8b6c4-x2kqp",
    ghosts: ["-worker-5c8d", "-gateway", "-api-v2"],
  },
  compose: {
    output: ["$ ls", "README.md  docker-compose.prod.yml  docker-compose.yml  src/  scripts/"],
    source: "docker-compose.prod.yml",
    typed: "docker compose -f docker-compose.p",
    completion: "rod.yml up -d",
    ghosts: ["docker-compose.yml", "README.md", "scripts/"],
  },
  git: {
    output: [
      "$ git branch -a",
      "  main",
      "* develop",
      "  feat/payments-refactor",
      "  remotes/origin/feat/payments-refactor",
    ],
    source: "feat/payments-refactor",
    typed: "git checkout feat/p",
    completion: "ayments-refactor",
    ghosts: ["main", "develop", "origin/feat/payments-refactor"],
  },
  trace: {
    output: [
      "$ python -m app",
      "Traceback (most recent call last):",
      '  File "/srv/app/src/billing/invoice_service.py", line 142, in total',
      "    return sum(l.amount for l in lines)",
      "TypeError: unsupported operand type(s)",
    ],
    source: "src/billing/invoice_service.py",
    typed: "nvim src/b",
    completion: "illing/invoice_service.py",
    ghosts: ["src/", "scripts/", "README.md"],
  },
  lsof: {
    output: [
      "$ lsof -i :3000",
      "COMMAND   PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME",
      "node    48213 dev    23u  IPv6 0x1a2b      0t0  TCP *:hbci (LISTEN)",
    ],
    source: "48213",
    typed: "kill ",
    completion: "48213",
    ghosts: ["node", "3000", "-9"],
  },
  docker: {
    output: [
      "$ docker ps",
      "CONTAINER ID   IMAGE         NAMES",
      "3f2a9c1b7e4d   postgres:16   pgl-db-1",
      "9b8e7d6c5a4f   redis:7       pgl-cache-1",
    ],
    source: "pgl-db-1",
    typed: "docker exec -it pgl-d",
    completion: "b-1 bash",
    ghosts: ["pgl-cache-1", "postgres:16", "redis:7"],
  },
};

const ORDER = Object.keys(SCENARIOS);
const BEAT_MS = 3400;
// Headless captures and first paint show the approved comp; cycling starts after a pause.
const FIRST_STRIKE_MS = 5200;
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function escapeHtml(text) {
  return text.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

function setupHeroCycle() {
  const tube = document.querySelector("[data-cycle]");
  if (!tube || reducedMotion) return;
  const typed = tube.querySelector("[data-typed]");
  const ghosts = tube.querySelector("[data-ghosts]");
  const struck = tube.querySelector("[data-struck]");
  const afterglow = tube.querySelector("[data-afterglow]");
  const hero = tube.closest(".hero");
  let index = 0;
  let paused = false;
  let loops = 0;

  const strike = (name) => {
    const scenario = SCENARIOS[name];
    afterglow.textContent = struck.textContent;
    afterglow.classList.remove("decaying");
    void afterglow.offsetWidth;
    afterglow.classList.add("decaying");
    typed.textContent = scenario.typed.trimEnd() || scenario.typed;
    ghosts.innerHTML = scenario.ghosts.map((g) => `<li>${escapeHtml(g)}</li>`).join("");
    struck.textContent = scenario.completion;
  };

  const tick = () => {
    if (!paused) {
      index = (index + 1) % ORDER.length;
      if (index === 0) loops += 1;
      strike(ORDER[index]);
    }
    // One full pass, then rest on the first example.
    if (loops < 1) window.setTimeout(tick, BEAT_MS);
  };

  hero.addEventListener("pointerenter", () => { paused = true; });
  hero.addEventListener("pointerleave", () => { paused = false; });
  hero.addEventListener("focusin", () => { paused = true; });
  hero.addEventListener("focusout", () => { paused = false; });
  window.setTimeout(tick, FIRST_STRIKE_MS);
}

function setupScreen() {
  const screen = document.querySelector("[data-screen]");
  if (!screen) return;
  const tabs = [...screen.querySelectorAll("[data-scenario]")];
  const output = screen.querySelector("[data-output]");
  const typed = screen.querySelector("[data-prompt-typed]");
  const completion = screen.querySelector("[data-prompt-completion]");

  const show = (name) => {
    const scenario = SCENARIOS[name];
    const lines = scenario.output.map(escapeHtml);
    const source = escapeHtml(scenario.source);
    // Light the first occurrence of the name, where wisp found it.
    let lit = false;
    output.innerHTML = lines
      .map((line) => {
        if (lit || !line.includes(source)) return line;
        lit = true;
        return line.replace(source, `<mark>${source}</mark>`);
      })
      .join("\n");
    typed.textContent = scenario.typed;
    completion.textContent = scenario.completion;
    for (const tab of tabs) {
      const selected = tab.dataset.scenario === name;
      tab.setAttribute("aria-selected", String(selected));
      tab.tabIndex = selected ? 0 : -1;
    }
  };

  tabs.forEach((tab, i) => {
    tab.addEventListener("click", () => show(tab.dataset.scenario));
    tab.addEventListener("keydown", (event) => {
      const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
      if (!step) return;
      event.preventDefault();
      const next = tabs[(i + step + tabs.length) % tabs.length];
      next.focus();
      show(next.dataset.scenario);
    });
  });
  show(ORDER[0]);
}

function setupCopy() {
  for (const button of document.querySelectorAll("[data-copy]")) {
    button.addEventListener("click", async () => {
      const text = button.parentElement.querySelector("code").textContent;
      try {
        await navigator.clipboard.writeText(text);
        button.textContent = "Copied";
      } catch {
        button.textContent = "Select and copy";
      }
      button.classList.add("copied");
      window.setTimeout(() => {
        button.textContent = "Copy";
        button.classList.remove("copied");
      }, 1600);
    });
  }
}

setupHeroCycle();
setupScreen();
setupCopy();
