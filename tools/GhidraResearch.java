import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSetView;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class GhidraResearch extends GhidraScript {
    private static final long[] GLOBALS = {
        0x00B74490L, 0x00B74548L, 0x00B70152L, 0x00B70153L,
        0x00C81320L, 0x00C81324L
    };

    private void printReferences(long value) {
        Address target = toAddr(value);
        println(String.format("TARGET 0x%08X", value));
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target);
        int count = 0;
        while (refs.hasNext() && count < 80) {
            Reference ref = refs.next();
            Function function = currentProgram.getFunctionManager()
                .getFunctionContaining(ref.getFromAddress());
            String functionName = function == null
                ? "<no function>"
                : function.getName() + "@0x" + function.getEntryPoint();
            println(String.format(
                "  REF 0x%s type=%s function=%s",
                ref.getFromAddress(), ref.getReferenceType(), functionName));
            count++;
        }
        println("  REF_COUNT_PRINTED " + count);
    }

    private void printFunction(long value) {
        Function function = currentProgram.getFunctionManager()
            .getFunctionAt(toAddr(value));
        if (function == null) {
            function = currentProgram.getFunctionManager()
                .getFunctionContaining(toAddr(value));
        }
        if (function == null) {
            println(String.format("FUNCTION 0x%08X <none>", value));
            return;
        }
        println(String.format(
            "FUNCTION 0x%08X entry=0x%s name=%s body=%s",
            value, function.getEntryPoint(), function.getName(), function.getBody()));
        AddressSetView body = function.getBody();
        Listing listing = currentProgram.getListing();
        int printed = 0;
        for (CodeUnit unit : listing.getCodeUnits(body, true)) {
            if (unit instanceof Instruction) {
                println("  " + unit.getAddress() + ": " + unit);
                printed++;
                if (printed >= 120) {
                    println("  ...");
                    break;
                }
            }
        }
    }

    private void scanOffsetInstructions() {
        Listing listing = currentProgram.getListing();
        int count = 0;
        for (Instruction instruction : listing.getInstructions(true)) {
            String text = instruction.toString().toLowerCase();
            if (text.contains("0xb8") || text.contains("+b8") ||
                text.contains("0x484") || text.contains("+484")) {
                Function function = currentProgram.getFunctionManager()
                    .getFunctionContaining(instruction.getAddress());
                String name = function == null ? "<no function>" : function.getName();
                println(String.format(
                    "OFFSET_REF %s function=%s instruction=%s",
                    instruction.getAddress(), name, instruction));
                count++;
                if (count >= 300) {
                    println("OFFSET_REF_LIMIT");
                    return;
                }
            }
        }
        println("OFFSET_REF_COUNT " + count);
    }

    @Override
    public void run() throws Exception {
        println("GHIDRA_RESEARCH_START " + currentProgram.getExecutablePath());
        FunctionManager manager = currentProgram.getFunctionManager();
        println("FUNCTION_COUNT " + manager.getFunctionCount());

        for (long global : GLOBALS) {
            printReferences(global);
        }

        long[] candidates = {
            0x00439B20L, 0x0043A0B0L, 0x0053BEE0L,
            0x00588BE0L, 0x0058A330L, 0x0069F1E0L
        };
        for (long candidate : candidates) {
            printFunction(candidate);
        }

        scanOffsetInstructions();
        println("GHIDRA_RESEARCH_END");
    }
}
