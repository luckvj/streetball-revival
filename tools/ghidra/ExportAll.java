// Export decompiled C for every function, strings with referencing functions, and the call graph.
// Output directory is the first script argument.
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.address.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.data.*;
import java.io.*;
import java.util.*;

public class ExportAll extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String out = getScriptArgs().length > 0 ? getScriptArgs()[0] : ".";
        new File(out).mkdirs();
        Listing listing = currentProgram.getListing();
        FunctionManager fm = currentProgram.getFunctionManager();
        ReferenceManager rm = currentProgram.getReferenceManager();

        // Call graph and function list.
        try (PrintWriter w = new PrintWriter(new FileWriter(new File(out, "functions.txt")))) {
            for (Function f : fm.getFunctions(true)) {
                StringBuilder callers = new StringBuilder();
                Set<String> seen = new TreeSet<>();
                for (Reference r : rm.getReferencesTo(f.getEntryPoint())) {
                    Function c = fm.getFunctionContaining(r.getFromAddress());
                    if (c != null) seen.add(c.getEntryPoint().toString());
                }
                for (String s : seen) callers.append(' ').append(s);
                w.printf("%s %d %s callers:%s%n", f.getEntryPoint(), f.getBody().getNumAddresses(), f.getName(), callers);
            }
        }

        // Strings and the functions that reference them.
        try (PrintWriter w = new PrintWriter(new FileWriter(new File(out, "strings.txt")))) {
            for (Data d : listing.getDefinedData(true)) {
                if (!d.hasStringValue()) continue;
                Object v = d.getValue();
                if (v == null) continue;
                String s = v.toString().replace("\n", "\\n").replace("\r", "\\r");
                Set<String> users = new TreeSet<>();
                for (Reference r : rm.getReferencesTo(d.getAddress())) {
                    Function c = fm.getFunctionContaining(r.getFromAddress());
                    users.add(c != null ? c.getEntryPoint().toString() : ("@" + r.getFromAddress()));
                }
                w.printf("%s [%s] %s%n", d.getAddress(), String.join(",", users), s);
            }
        }

        // Decompile everything.
        DecompInterface di = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        di.setOptions(opts);
        di.toggleCCode(true);
        di.toggleSyntaxTree(false);
        di.openProgram(currentProgram);
        int n = 0, failed = 0;
        try (PrintWriter w = new PrintWriter(new FileWriter(new File(out, "decomp.c")))) {
            for (Function f : fm.getFunctions(true)) {
                if (monitor.isCancelled()) break;
                DecompileResults res = di.decompileFunction(f, 60, monitor);
                w.printf("%n//==== %s %s%n", f.getEntryPoint(), f.getName());
                if (res != null && res.decompileCompleted()) {
                    w.print(res.getDecompiledFunction().getC());
                } else {
                    failed++;
                    w.printf("// decompile failed: %s%n", res == null ? "null" : res.getErrorMessage());
                }
                n++;
                if (n % 500 == 0) println("decompiled " + n);
            }
        }
        println("done: " + n + " functions, " + failed + " failed");
    }
}
