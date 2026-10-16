// Decompile functions and their callees (breadth-first, up to a depth) to C text files, plus an
// assembly listing of each (for checking f32 vs f64 arithmetic: mulss vs mulsd).
// Usage (headless): -postScript DecompileTree.java <out-dir> <depth> <0xaddr|name> [...]
// Callees outside the root functions' neighbourhood are still followed; library thunks are skipped.
// Output: <out-dir>/<addr>_<name>.c and .asm, and <out-dir>/index.tsv (addr, name, depth, callees).
// @category IG2

import java.io.File;
import java.io.PrintWriter;
import java.util.*;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.Symbol;

public class DecompileTree extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args[0]);
        outDir.mkdirs();
        int maxDepth = Integer.parseInt(args[1]);
        DecompInterface di = new DecompInterface();
        di.setOptions(new DecompileOptions());
        di.openProgram(currentProgram);
        Map<Function, Integer> depth = new LinkedHashMap<>();
        Map<Function, Set<String>> callers = new HashMap<>();
        Deque<Function> queue = new ArrayDeque<>();
        for (int i = 2; i < args.length; i++) {
            Function f = lookup(args[i]);
            if (f == null) { println("not found: " + args[i]); continue; }
            depth.put(f, 0);
            queue.add(f);
        }
        try (PrintWriter idx = new PrintWriter(new File(outDir, "index.tsv"))) {
            while (!queue.isEmpty()) {
                Function f = queue.poll();
                int d = depth.get(f);
                String base = f.getEntryPoint().toString() + "_" + f.getName(true).replaceAll("[^A-Za-z0-9_.-]", "_");
                DecompileResults r = di.decompileFunction(f, 300, monitor);
                String c = r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed: " + r.getErrorMessage();
                try (PrintWriter w = new PrintWriter(new File(outDir, base + ".c"))) {
                    w.println("// " + f.getName(true) + " @ " + f.getEntryPoint() + " (" + currentProgram.getName() + ")");
                    w.print(c);
                }
                try (PrintWriter w = new PrintWriter(new File(outDir, base + ".asm"))) {
                    for (Instruction ins : currentProgram.getListing().getInstructions(f.getBody(), true)) {
                        w.println(ins.getAddress() + "  " + ins.toString());
                    }
                }
                List<String> cs = new ArrayList<>();
                for (Function g : f.getCalledFunctions(monitor)) cs.add(g.getEntryPoint().toString());
                Collections.sort(cs);
                idx.println(f.getEntryPoint() + "\t" + f.getName(true) + "\t" + d + "\t" + String.join(",", cs));
                idx.flush();
                if (d >= maxDepth) continue;
                for (Function g : f.getCalledFunctions(monitor)) {
                    if (g.isThunk() || g.isExternal()) continue;
                    callers.computeIfAbsent(g, k -> new TreeSet<>()).add(f.getEntryPoint().toString());
                    if (!depth.containsKey(g)) {
                        depth.put(g, d + 1);
                        queue.add(g);
                    }
                }
            }
        }
        di.dispose();
    }

    private Function lookup(String name) {
        if (name.startsWith("0x")) return getFunctionContaining(toAddr(name.substring(2)));
        String simple = name.contains("::") ? name.substring(name.lastIndexOf("::") + 2) : name;
        for (Symbol s : currentProgram.getSymbolTable().getSymbols(simple)) {
            Function cand = getFunctionAt(s.getAddress());
            if (cand != null && (!name.contains("::") || cand.getName(true).equals(name))) return cand;
        }
        return null;
    }
}
