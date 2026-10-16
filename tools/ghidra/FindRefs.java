// List the references to data addresses (or anywhere inside a range) with the function that makes each.
// Usage (headless): -postScript FindRefs.java <out-file> <0xaddr[:len]> [...]
// Output: one line per reference: target, from address, reference type, containing function.
// @category IG2

import java.io.File;
import java.io.PrintWriter;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class FindRefs extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        try (PrintWriter out = new PrintWriter(new File(args[0]))) {
            for (int i = 1; i < args.length; i++) {
                String[] parts = args[i].split(":");
                long start = Long.decode(parts[0]);
                long len = parts.length > 1 ? Long.decode(parts[1]) : 1;
                for (long a = start; a < start + len; a++) {
                    Address addr = toAddr(a);
                    for (Reference r : getReferencesTo(addr)) {
                        Function f = getFunctionContaining(r.getFromAddress());
                        out.printf("%s\t%s\t%s\t%s%n", addr, r.getFromAddress(), r.getReferenceType(),
                            f == null ? "?" : f.getEntryPoint() + " " + f.getName());
                    }
                }
            }
        }
    }
}
