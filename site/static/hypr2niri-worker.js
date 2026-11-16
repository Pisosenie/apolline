// The converter runs here, apart from the page, so a config that never ends can't hang it.
// hyprland.conf goes straight to hypr2niri's WebAssembly; hyprland.lua first runs in Lua
// (wasmoon) under the same sandbox the command line uses, and what it noted goes there.

const WASMOON = "https://cdn.jsdelivr.net/npm/wasmoon@1.16.0/dist/";
let converter;
let sandbox;

async function ready() {
  if (!converter) {
    const { instance } = await WebAssembly.instantiateStreaming(fetch("hypr2niri/hypr2niri.wasm"), {});
    converter = instance.exports;
  }
  return converter;
}

function call(name, text) {
  const wasm = converter;
  const bytes = new TextEncoder().encode(text);
  const ptr = wasm.alloc(Math.max(bytes.length, 1));
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
  const packed = BigInt.asUintN(64, wasm[name](ptr, bytes.length));
  const at = Number(packed >> 32n);
  const len = Number(packed & 0xffffffffn);
  const out = new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, at, len));
  wasm.release(at, len);
  return out;
}

const RUN = `
assert(load(__sandbox_text, "=import-sandbox"))()
local ok, err = pcall(assert(load(__config_text, "@hyprland.lua")))
local out = {}
for _, n in ipairs(__import_notes) do
  if not n.off then
    out[#out + 1] = table.concat({ n.key, n.value, n.at, tostring(n.block or 0) }, "\\31")
  end
end
return table.concat(out, "\\n") .. "\\30" .. ((not ok) and tostring(err) or "")
`;

async function lua(text) {
  if (!self.wasmoon) importScripts(WASMOON + "index.js");
  sandbox ??= await (await fetch("hypr2niri/sandbox.lua")).text();
  const factory = new self.wasmoon.LuaFactory(WASMOON + "glue.wasm");
  const engine = await factory.createEngine();
  try {
    engine.global.set("__import_dir", ".");
    engine.global.set("__sandbox_text", sandbox);
    engine.global.set("__config_text", text);
    const [noted, stopped] = String(await engine.doString(RUN)).split("\x1e");
    return { out: call("convert_lua", noted), stopped };
  } finally {
    engine.global.close();
  }
}

self.onmessage = async ({ data }) => {
  try {
    await ready();
    const result = data.lua ? await lua(data.text) : { out: call("convert_conf", data.text) };
    self.postMessage({ ok: true, ...result });
  } catch (error) {
    self.postMessage({ ok: false, error: String(error && error.message || error) });
  }
};
