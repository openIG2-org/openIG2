// Decompile named functions to C text files.
// Usage (headless): -postScript DecompileFunctions.java <out-dir> <name1> [<name2> ...]
// A name may be a plain or namespaced symbol (e.g. LoadObject, AssetHelper::OpenFile) or 0x<addr>.
// Output: <out-dir>/<sanitized-name>.c
// @category IG2

import java.io.File;
import java.io.PrintWriter;
import java.util.List;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Symbol;

public class DecompileFunctions extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args[0]);
        outDir.mkdirs();
        DecompInterface di = new DecompInterface();
        di.setOptions(new DecompileOptions());
        di.openProgram(currentProgram);
        for (int i = 1; i < args.length; i++) {
            String name = args[i];
            Function f = null;
            if (name.startsWith("0x")) {
                f = getFunctionContaining(toAddr(name.substring(2)));
            } else {
                List<Symbol> syms = getSymbols(name.contains("::") ? name.substring(name.lastIndexOf("::") + 2) : name,
                        null);
                for (Symbol s : currentProgram.getSymbolTable().getSymbols(
                        name.contains("::") ? name.substring(name.lastIndexOf("::") + 2) : name)) {
                    Function cand = getFunctionAt(s.getAddress());
                    if (cand != null && (!name.contains("::") || cand.getName(true).equals(name))) {
                        f = cand;
                        break;
                    }
                }
            }
            if (f == null) {
                println("not found: " + name);
                continue;
            }
            DecompileResults r = di.decompileFunction(f, 300, monitor);
            String c = r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed: " + r.getErrorMessage();
            File out = new File(outDir, name.replaceAll("[^A-Za-z0-9_.-]", "_") + ".c");
            try (PrintWriter w = new PrintWriter(out)) {
                w.println("// " + f.getName(true) + " @ " + f.getEntryPoint() + " (" + currentProgram.getName() + ")");
                w.print(c);
            }
            println("wrote " + out);
        }
        di.dispose();
    }
}
