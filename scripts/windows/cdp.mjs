// Drives a Chromium page over the DevTools protocol for real-machine QA: no input reaches the
// (shared) console, nothing takes focus. winer's QA build opens its port with WINER_DEVTOOLS_PORT
// (scripts/windows/qa.sh deploy); WINER_CDP_PORT picks another port and WINER_CDP_MATCH a target
// whose URL contains it. From any session:
//   node cdp.mjs eval "<expression>"          prints the JSON result
//   node cdp.mjs shot <file.png> [expression]  runs the expression first (e.g. a click), then shoots
//   node cdp.mjs click "<css selector>"        a page-level mouse click at the element's centre
//   node cdp.mjs clickAt <x> <y>               the same at a point, in CSS pixels
import { writeFileSync } from "node:fs";

const PORT = Number(process.env.WINER_CDP_PORT ?? 9333);
const [command, ...rest] = process.argv.slice(2);

const targets = await fetch(`http://127.0.0.1:${PORT}/json`).then((response) => response.json());
const match = process.env.WINER_CDP_MATCH ?? "";
const page = targets.find((target) => target.type === "page" && target.url.includes(match));
if (!page) throw new Error(`no page target on port ${PORT}`);

const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.addEventListener("open", resolve, { once: true });
  socket.addEventListener("error", reject, { once: true });
});

let nextId = 0;
const pending = new Map();
socket.addEventListener("message", (message) => {
  const data = JSON.parse(String(message.data));
  const waiter = pending.get(data.id);
  if (!waiter) return;
  pending.delete(data.id);
  if (data.error) waiter.reject(new Error(JSON.stringify(data.error)));
  else waiter.resolve(data.result);
});

function send(method, params = {}) {
  const id = ++nextId;
  socket.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => {
    // Cleared by the answer: a timer left pending keeps the process alive for its whole 15 s.
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`${method} timed out`));
    }, 15_000);
    const settle = (finish) => (value) => {
      clearTimeout(timer);
      finish(value);
    };
    pending.set(id, { resolve: settle(resolve), reject: settle(reject) });
  });
}

async function evaluate(expression) {
  const result = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails)
    throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
}

try {
  if (command === "eval") {
    console.log(JSON.stringify(await evaluate(rest.join(" ")), null, 1));
  } else if (command === "shot") {
    const [file, ...expression] = rest;
    if (expression.length > 0) await evaluate(expression.join(" "));
    // Let a page switch settle and lazy images land.
    await evaluate("new Promise((resolve) => setTimeout(resolve, 700))");
    const { data } = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(file, Buffer.from(data, "base64"));
    console.log(`saved ${file}`);
  } else if (command === "click" || command === "clickAt") {
    // Dispatched into the page by the browser, not through the OS: the console's cursor never moves.
    const selector = JSON.stringify(rest.join(" "));
    const point =
      command === "clickAt"
        ? { x: Number(rest[0]), y: Number(rest[1]) }
        : await evaluate(
            `(() => { const element = document.querySelector(${selector}); if (!element) return null; const box = element.getBoundingClientRect(); return { x: box.x + box.width / 2, y: box.y + box.height / 2 }; })()`,
          );
    if (!point || !Number.isFinite(point.x) || !Number.isFinite(point.y)) {
      throw new Error(`nothing to click at ${rest.join(" ")}`);
    }
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
      await send("Input.dispatchMouseEvent", {
        type,
        x: point.x,
        y: point.y,
        button: "left",
        clickCount: type === "mouseMoved" ? 0 : 1,
      });
    }
    console.log(JSON.stringify(point));
  } else {
    throw new Error(
      "usage: node cdp.mjs eval <expression> | shot <file.png> [expression] | click <selector> | clickAt <x> <y>",
    );
  }
} finally {
  socket.close();
}
