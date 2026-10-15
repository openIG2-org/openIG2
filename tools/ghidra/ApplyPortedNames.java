// Apply ELF-derived names (tools/symport/match_symbols.py output) to the PE program.
// Usage (headless, on ig2.exe): -postScript ApplyPortedNames.java <matches.tsv>
// Names with "::" are placed into (created) namespaces. Each function gets a plate comment
// with the ELF address and match method so every ported name stays auditable.
// @category IG2

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.SymbolTable;

public class ApplyPortedNames extends GhidraScript {
    @Override
    protected void run() throws Exception {
        List<String> lines = Files.readAllLines(Paths.get(getScriptArgs()[0]));
        SymbolTable st = currentProgram.getSymbolTable();
        int ok = 0, skipped = 0;
        for (String line : lines.subList(1, lines.size())) {
            String[] c = line.split("\t");
            Address a = toAddr(c[0]);
            Function f = getFunctionAt(a);
            if (f == null) { skipped++; continue; }
            // Keep any name a human already set.
            if (f.getSymbol().getSource() == SourceType.USER_DEFINED) { skipped++; continue; }
            String full = c[2];
            String[] parts = full.split("::");
            Namespace ns = currentProgram.getGlobalNamespace();
            try {
                for (int i = 0; i < parts.length - 1; i++) {
                    Namespace next = st.getNamespace(parts[i], ns);
                    ns = next != null ? next : st.createNameSpace(ns, parts[i], SourceType.IMPORTED);
                }
                f.setParentNamespace(ns);
                f.setName(parts[parts.length - 1], SourceType.IMPORTED);
                f.setComment("Ported from unstripped ELF ig2 @0x" + c[1] + " (" + c[3] + ")");
                ok++;
            } catch (Exception e) {
                println("failed " + c[0] + " " + full + ": " + e.getMessage());
                skipped++;
            }
        }
        println("applied " + ok + " names, skipped " + skipped);
    }
}
