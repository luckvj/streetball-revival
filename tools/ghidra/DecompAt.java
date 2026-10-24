// Decompile the code at given addresses, creating functions where Ghidra
// has none (handlers reached only through data tables).
// Arguments: <out file> <hex address>...
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.program.model.listing.*;
import ghidra.program.model.address.*;
import java.io.*;

public class DecompAt extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        FunctionManager fm = currentProgram.getFunctionManager();
        DecompInterface di = new DecompInterface();
        di.setOptions(new DecompileOptions());
        di.toggleCCode(true);
        di.openProgram(currentProgram);
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            for (int i = 1; i < args.length; i++) {
                Address a = toAddr(Long.parseLong(args[i], 16));
                if (getInstructionAt(a) == null) {
                    new DisassembleCommand(a, null, true).applyTo(currentProgram);
                }
                Function f = fm.getFunctionAt(a);
                if (f == null) {
                    Function inside = fm.getFunctionContaining(a);
                    if (inside != null) {
                        w.printf("//==== %s inside %s%n", a, inside.getEntryPoint());
                    }
                    f = createFunction(a, null);
                }
                if (f == null) {
                    w.printf("//==== %s (no function)%n%n", a);
                    continue;
                }
                DecompileResults r = di.decompileFunction(f, 60, monitor);
                w.printf("//==== %s %s%n", f.getEntryPoint(), f.getName());
                w.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// failed: " + r.getErrorMessage());
            }
        }
    }
}
