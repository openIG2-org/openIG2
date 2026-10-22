"""The remaster's screen layout at several window sizes (re/screens/widescreen.md).

    probe_layout.py <install> <out.json> WxH [WxH ...]

Spawns ig2.exe once per size (ig2.settings width/height, put back afterwards), waits 25 s and reads `ig2_resize`'s
(exe 0x4d84d0) window and virtual-frame globals and `ModifyViewPort`'s (0x4ac4f0) `Hatar` range (only set once a 3D
screen has been entered: often still 0 on the title).
"""
import json, os, re, shutil, sys, time
import frida
inst = sys.argv[1]
st = os.path.join(inst, "ig2.settings")
JS = r"""
const b = Process.getModuleByName('ig2.exe').base.sub(0x400000);
const r = a => b.add(a).readS32();
rpc.exports.read = () => ({
  sdl: [r(0xb7721c), r(0xb772a0)], full: [r(0xb770b8), r(0xb770bc)],
  win: [r(0xb770c8), r(0xb770cc), r(0xb77090), r(0xb77094)],
  virt: [r(0xb770d8), r(0xb770dc), r(0xb770d0), r(0xb770d4)],
  hatar: [r(0xb9a980), r(0xb9a984), r(0xb9a988), r(0xb9a98c), r(0xb9a990), r(0xb9a994)],
  resizes: r(0xb77098), toolbar: b.add(0xfe6532).readU8(),
});
"""
sizes = [s.lower().split("x") for s in sys.argv[3:]]
bak = st + ".layout-bak"
shutil.copyfile(st, bak)
orig = open(bak, "rb").read()
out = []
try:
    for w, h in sizes:
        s = re.sub(rb"(?m)^width=\d+", b"width=" + w.encode(), orig)
        s = re.sub(rb"(?m)^height=\d+", b"height=" + h.encode(), s)
        open(st, "wb").write(s)
        pid = frida.spawn([os.path.join(inst, "ig2.exe")], cwd=inst)
        sess = frida.attach(pid)
        sc = sess.create_script(JS); sc.load(); frida.resume(pid)
        time.sleep(25)
        v = sc.exports_sync.read(); v["size"] = [int(w), int(h)]
        print(json.dumps(v)); out.append(v)
        frida.kill(pid)
        time.sleep(2)
finally:
    open(st, "wb").write(orig); os.remove(bak)
json.dump(out, open(sys.argv[2], "w"), indent=1)
