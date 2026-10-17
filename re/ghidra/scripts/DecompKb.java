// Decompiles functions to <outDir>/<addr9>.c. Args: <outDir> <kb stamp> <addr[:label]>...
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.*;
import ghidra.program.model.symbol.*;
import java.nio.file.*;

public class DecompKb extends GhidraScript {
    boolean looksString(Address a) {
        try {
            Memory m = currentProgram.getMemory();
            int n = 0;
            while (n < 512) {
                byte b = m.getByte(a.add(n));
                if (b == 0) break;
                if (b < 0x09 || (b > 0x0d && b < 0x20) || b == 0x7f) return false;
                n++;
            }
            return n >= 3;
        } catch (Exception e) { return false; }
    }

    void defineStrings(Function f) {
        Listing l = currentProgram.getListing();
        MemoryBlock xt = currentProgram.getMemory().getBlock(".xtext");
        InstructionIterator it = l.getInstructions(f.getBody(), true);
        while (it.hasNext()) {
            for (Reference r : it.next().getReferencesFrom()) {
                Address t = r.getToAddress();
                if (xt != null && xt.contains(t) && l.getDefinedDataAt(t) == null && looksString(t)) {
                    try { createAsciiString(t); } catch (Exception e) {}
                }
            }
        }
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String outDir = args[0], stamp = args[1];
        DecompInterface di = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        opts.setMaxPayloadMBytes(512);
        di.setOptions(opts);
        di.toggleCCode(true);
        di.toggleSyntaxTree(true);
        di.openProgram(currentProgram);
        for (int i = 2; i < args.length; i++) {
            String spec = args[i].split(":")[0].toLowerCase().replace("0x", "");
            try {
                Address a = toAddr(Long.parseUnsignedLong(spec, 16));
                Function f = getFunctionAt(a);
                if (f == null || f.getBody().getNumAddresses() <= 1) {
                    if (currentProgram.getListing().getInstructionAt(a) == null)
                        new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
                    if (f == null) new CreateFunctionCmd(a).applyTo(currentProgram, monitor);
                    else CreateFunctionCmd.fixupFunctionBody(currentProgram, f, monitor);
                    f = getFunctionAt(a);
                }
                if (f == null) { println("NOFUNC " + a); continue; }
                defineStrings(f);
                DecompileResults res = di.decompileFunction(f, 180, monitor);
                String c = res.decompileCompleted() ? res.getDecompiledFunction().getC() : ("// FAILED " + res.getErrorMessage() + "\n");
                Path out = Paths.get(outDir, a.toString() + ".c");
                Files.writeString(out, "// " + a + " " + f.getName() + " body " + f.getBody().getMinAddress() + "-"
                    + f.getBody().getMaxAddress() + " kb=" + stamp + "\n" + c);
                println("OK " + a + " " + f.getName() + " -> " + out);
            } catch (Exception e) {
                println("ERR " + spec + " " + e);
            }
        }
        di.dispose();
    }
}
