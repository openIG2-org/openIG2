// Export per-function features for cross-binary symbol matching (ELF ig2 -> PE ig2.exe).
// Usage (headless): -postScript ExportFunctionFeatures.java <out.jsonl>
// Output: one JSON object per function:
//   entry, name, size, ninsn, strings[], consts[], callees[] (entry addresses), source (symbol source)
// @category IG2

import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;

public class ExportFunctionFeatures extends GhidraScript {

    private static String esc(String s) {
        StringBuilder b = new StringBuilder();
        for (char c : s.toCharArray()) {
            switch (c) {
                case '"': b.append("\\\""); break;
                case '\\': b.append("\\\\"); break;
                case '\n': b.append("\\n"); break;
                case '\r': b.append("\\r"); break;
                case '\t': b.append("\\t"); break;
                default:
                    if (c < 0x20 || c > 0x7e) b.append(String.format("\\u%04x", (int) c));
                    else b.append(c);
            }
        }
        return b.toString();
    }

    private String rawCString(Address a) {
        ghidra.program.model.mem.MemoryBlock b = currentProgram.getMemory().getBlock(a);
        if (b == null || b.isExecute() || !b.isInitialized()) return null;
        StringBuilder sb = new StringBuilder();
        try {
            for (int i = 0; i < 512; i++) {
                int c = currentProgram.getMemory().getByte(a.add(i)) & 0xff;
                if (c == 0) break;
                if ((c < 0x20 && c != '\n' && c != '\r' && c != '\t') || c > 0x7e) return null;
                sb.append((char) c);
            }
        } catch (Exception e) {
            return null;
        }
        return sb.length() >= 3 ? sb.toString() : null;
    }

    /** Global data the function touches (not code, not strings). */
    private void addData(Set<String> out, Address a) {
        ghidra.program.model.mem.MemoryBlock b = currentProgram.getMemory().getBlock(a);
        if (b == null || b.isExecute()) return;
        if (getFunctionAt(a) != null) return;
        out.add(a.toString());
    }

    private String stringAt(Address a) {
        Data d = getDataAt(a);
        if (d == null || !d.isDefined()) return rawCString(a);
        if (d.hasStringValue()) {
            Object v = d.getValue();
            return v == null ? null : v.toString();
        }
        // One level of pointer indirection (string tables).
        if (d.isPointer()) {
            Object v = d.getValue();
            if (v instanceof Address) {
                Data d2 = getDataAt((Address) v);
                if (d2 != null && d2.hasStringValue() && d2.getValue() != null) return d2.getValue().toString();
            }
        }
        return null;
    }

    @Override
    protected void run() throws Exception {
        String out = getScriptArgs().length > 0 ? getScriptArgs()[0] : "features.jsonl";
        Memory mem = currentProgram.getMemory();
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        int n = 0;
        try (PrintWriter w = new PrintWriter(new FileWriter(out))) {
            for (Function f : fm.getFunctions(true)) {
                if (monitor.isCancelled()) break;
                if (f.isThunk() || f.isExternal()) continue;
                List<String> strings = new ArrayList<>();
                TreeSet<Long> consts = new TreeSet<>();
                LinkedHashSet<String> callees = new LinkedHashSet<>();
                LinkedHashSet<String> data = new LinkedHashSet<>();
                int ninsn = 0;
                for (Instruction ins : listing.getInstructions(f.getBody(), true)) {
                    ninsn++;
                    for (Reference r : ins.getReferencesFrom()) {
                        Address to = r.getToAddress();
                        if (r.getReferenceType().isCall()) {
                            Function c = fm.getFunctionAt(to);
                            if (c != null && c.isThunk()) c = c.getThunkedFunction(true);
                            if (c != null) callees.add(c.isExternal() ? "ext:" + c.getName() : c.getEntryPoint().toString());
                        } else if (r.getReferenceType().isData() || r.getReferenceType().isRead()) {
                            String s = stringAt(to);
                            if (s != null && s.length() >= 3) strings.add(s);
                            else addData(data, to);
                        }
                    }
                    for (int op = 0; op < ins.getNumOperands(); op++) {
                        for (Object o : ins.getOpObjects(op)) {
                            if (o instanceof Scalar) {
                                long v = ((Scalar) o).getUnsignedValue() & 0xffffffffL;
                                // Skip small values and negative stack/frame offsets.
                                if (v < 0x100 || v >= 0xffff0000L) continue;
                                Address a = f.getEntryPoint().getNewAddress(v);
                                if (mem.contains(a)) {
                                    // Immediate address (clang `movabs`, MSVC `push offset`): keep it if it's a string.
                                    String s = stringAt(a);
                                    if (s != null && s.length() >= 3) strings.add(s);
                                    else addData(data, a);
                                    continue;
                                }
                                consts.add(v);
                            }
                        }
                    }
                }
                w.print("{\"entry\":\"" + f.getEntryPoint() + "\",\"name\":\"" + esc(f.getName(true))
                        + "\",\"source\":\"" + f.getSymbol().getSource() + "\",\"size\":" + f.getBody().getNumAddresses()
                        + ",\"ninsn\":" + ninsn + ",\"strings\":[");
                for (int i = 0; i < strings.size(); i++) w.print((i > 0 ? "," : "") + "\"" + esc(strings.get(i)) + "\"");
                w.print("],\"consts\":[");
                int i = 0;
                for (Long c : consts) w.print((i++ > 0 ? "," : "") + c);
                w.print("],\"data\":[");
                i = 0;
                for (String d : data) w.print((i++ > 0 ? "," : "") + "\"" + d + "\"");
                w.print("],\"callees\":[");
                i = 0;
                for (String c : callees) w.print((i++ > 0 ? "," : "") + "\"" + esc(c) + "\"");
                w.println("]}");
                n++;
            }
        }
        println("exported " + n + " functions to " + out);
    }
}
