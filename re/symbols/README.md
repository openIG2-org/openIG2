# Ported symbols: ELF `ig2` → PE `ig2.exe`

`ig2exe-ported-names.tsv` maps exe function entries to names recovered from the unstripped Linux
build. Columns: `exe_entry, elf_entry, name, method, exe_size, elf_size`.

## How it's produced

1. Headless Ghidra 12.1.4 project `_local/ghidra/ig2.gpr` (gitignored), with both binaries auto-analysed.
2. `tools/ghidra/ExportFunctionFeatures.java` exports per function: strings (references and
   immediate addresses), distinctive constants, global data refs, callees.
3. `tools/symport/match_symbols.py` matches them:
   - `str:n`: n strings unique to this function on both sides
   - `const:n`: n distinctive constants (>= 0x10000) unique on both sides
   - `callee` / `caller`: single-candidate gap between already-matched neighbours in the ordered call lists
   - `glob:n`: unique reference to a mapped global variable
   - `sim:s`: neighbourhood candidate with Jaccard score s over strings, constants, mapped globals and
     mapped callees, with a ≥1.5× margin over the runner-up
   Every non-string match must pass a compatibility check: no contradicting strings or constants, and
   a size ratio between 0.05 and 4.
4. `tools/ghidra/ApplyPortedNames.java` writes the names into the exe program, with a plate comment
   naming the ELF address and the method.

## Quality (run of 2026-10-15)

- 1,704 functions matched: str 595, callee 557, caller 315, sim 177, const 60.
- Contradiction check (pairs where both sides have strings or distinctive constants): 0 of 376 checkable pairs.
- Coverage is partial. Tiny ELF functions are often inlined by MSVC (`rndold`, `RandomInit` have
  no exe counterpart), and most ELF functions are STL instantiations.
- For Phase 2 tracing, missing targets get matched by hand and appended here with method `manual`.

## REA

REA's Windows Ghidra provider is read-only ("Windows Ghidra P0 does not admit database mutation"),
and its auto-analysis of `ig2.exe` exceeds the MCP request timeout. So the Ghidra project plus this
table are the naming source of truth. REA is used for runtime work (Frida) against these addresses.

## Known wrong names

- `0x49c780` -> `Build3dBuilding` is wrong: that function rebuilds a mesh's vertex list for `Init3dColony`; the real `Build3dBuilding` is exe `0x45f640` (ELF 0x4ca0b0).
- ELF `getbottomvertex` 0x47c750 is exe `0x4a89a0`; the exe's 0x47c750 is `KiszamolErtek` (a bare ELF address in a spec is not an exe address).
