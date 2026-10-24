// Recover callers truncated by an incorrectly inferred no-return singleton.
// Run against the dedicated Street project with -readOnly -noanalysis.
// Arguments: output.c, singleton entry, function entries to recover.
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.program.model.listing.*;
import ghidra.program.model.address.*;
import java.io.*;

public class RecoverAt extends GhidraScript {
    @Override protected void run() throws Exception {
        String[] args = getScriptArgs();
        FunctionManager manager = currentProgram.getFunctionManager();
        Address singleton = toAddr(Long.parseLong(args[1], 16));
        Function helper = manager.getFunctionAt(singleton);
        if (helper == null) throw new IllegalArgumentException("missing singleton function");
        helper.setNoReturn(false);
        for (int i=2; i<args.length; i++) {
            Address entry = toAddr(Long.parseLong(args[i],16));
            Function old = manager.getFunctionAt(entry);
            if (old == null) { println("missing " + entry); continue; }
            FunctionIterator following = manager.getFunctions(entry, true);
            Function next = null;
            while (following.hasNext()) {
                Function candidate = following.next();
                if (candidate.getEntryPoint().compareTo(entry) > 0) { next = candidate; break; }
            }
            if (next == null) throw new IllegalArgumentException("no next boundary");
            Address end = next.getEntryPoint().subtract(1);
            if (end.subtract(entry) > 0x10000) throw new IllegalArgumentException("unbounded function " + entry);
            String name = old.getName();
            manager.removeFunction(entry);
            clearListing(entry,end);
            new DisassembleCommand(entry,null,true).applyTo(currentProgram);
            Function recovered = createFunction(entry,name);
            if (recovered != null) recovered.setBody(new AddressSet(entry,end));
        }
        DecompInterface decoder = new DecompInterface();
        decoder.setOptions(new DecompileOptions());
        decoder.toggleCCode(true);
        decoder.openProgram(currentProgram);
        try(PrintWriter out = new PrintWriter(new FileWriter(args[0]))) {
            for(int i=2;i<args.length;i++) {
                Address entry = toAddr(Long.parseLong(args[i],16));
                Function function = manager.getFunctionAt(entry);
                out.printf("//==== %s %s%n",entry,function==null?"missing":function.getName());
                if (function==null) continue;
                DecompileResults result = decoder.decompileFunction(function,60,monitor);
                out.println(result.decompileCompleted()?result.getDecompiledFunction().getC():result.getErrorMessage());
            }
        } finally { decoder.dispose(); }
    }
}
