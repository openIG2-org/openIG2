"""Which language ig2.exe runs in (re/formats/locale.md §3): spawns the exe, logs the files it opens that pick
the language, GetLocaleInfoA answers and the 1999 code's `Lang` (exe 0x912724), then kills it.

    probe_lang.py <install> [seconds]
"""
import frida, sys, time
inst = sys.argv[1]
js = r"""
var base = Process.getModuleByName('ig2.exe').base;
function va(a){ return base.add(a - 0x400000); }
var seen = {};
['CreateFileA','CreateFileW'].forEach(function(n){
  var f = Process.getModuleByName('kernel32.dll').findExportByName(n);
  Interceptor.attach(f, { onEnter: function(a){
    var p = n.endsWith('W') ? a[0].readUtf16String() : a[0].readAnsiString();
    if (p && /locali|locale|text\.dat|init.config|settings|\.cfg/i.test(p) && !seen[p]) { seen[p]=1; send({open:p}); }
  }});
});
var gli = Process.getModuleByName('kernel32.dll').findExportByName('GetLocaleInfoA');
Interceptor.attach(gli, { onEnter: function(a){ this.b=a[2]; this.t=a[1].toInt32(); }, onLeave: function(r){ try { send({gli:this.t, v:this.b.readAnsiString()}); } catch(e){} }});
setInterval(function(){ send({lang: va(0x912724).readS32()}); }, 3000);
"""
dev = frida.get_local_device()
pid = dev.spawn([inst + '/ig2.exe'], cwd=inst)
s = dev.attach(pid); sc = s.create_script(js)
sc.on('message', lambda m, d: print(m.get('payload', m), flush=True))
sc.load(); dev.resume(pid)
time.sleep(float(sys.argv[2]) if len(sys.argv) > 2 else 20)
dev.kill(pid)
