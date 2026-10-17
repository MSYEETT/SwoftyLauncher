// Writes USER_DEFINED function names and labels that the KB does not have (or names differently) as L0 TSV rows.
// Args: <kb dir> <out.tsv>
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.nio.file.*;
import java.util.*;

public class KbExport extends GhidraScript {
    Map<String, String> kbNames(Path p) throws Exception {
        Map<String, String> m = new HashMap<>();
        if (!Files.exists(p)) return m;
        String[] hdr = null;
        for (String line : Files.readAllLines(p)) {
            if (line.isBlank() || line.startsWith("#")) continue;
            String[] c = line.split("\t", -1);
            if (hdr == null) { hdr = c; continue; }
            int ai = Arrays.asList(hdr).indexOf("addr"), ni = Arrays.asList(hdr).indexOf("name");
            if (ai < 0 || ni < 0 || c.length <= Math.max(ai, ni)) continue;
            m.put(c[ai].trim().toLowerCase().replace("0x", ""), c[ni].trim());
        }
        return m;
    }

    String clean(String s) { return s == null ? "-" : s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' '); }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        Path kb = Paths.get(args[0]);
        Map<String, String> fk = kbNames(kb.resolve("functions.tsv")), gk = kbNames(kb.resolve("globals.tsv"));
        List<String> fn = new ArrayList<>(), gl = new ArrayList<>();
        SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(true);
        while (it.hasNext()) {
            Symbol s = it.next();
            if (s.getSource() != SourceType.USER_DEFINED || s.isExternal()) continue;
            String key = s.getAddress().toString().toLowerCase();
            String a = "0x" + key;
            if (s.getSymbolType() == SymbolType.FUNCTION) {
                if (s.getName().equals(fk.get(key))) continue;
                String prev = fk.containsKey(key) ? "kb_name=" + fk.get(key) : "-";
                fn.add(String.join("\t", a, s.getName(), "-", "L0", "-", "-", "pat:ghidra-export", prev));
            } else if (s.getSymbolType() == SymbolType.LABEL) {
                if (s.getName().equals(gk.get(key)) || s.getName().equals(fk.get(key))) continue;
                String prev = gk.containsKey(key) ? "kb_name=" + gk.get(key) : "-";
                gl.add(String.join("\t", a, s.getName(), "-", "L0", "-", "-", "pat:ghidra-export", prev));
            }
        }
        StringBuilder sb = new StringBuilder();
        sb.append("# functions.tsv rows\naddr\tname\tsig\tlevel\tsubsystem\tproto\tevidence\tnotes\n");
        for (String r : fn) sb.append(r).append('\n');
        sb.append("# globals.tsv rows\naddr\tname\ttype\tlevel\tsubsystem\tsig\tevidence\tnotes\n");
        for (String r : gl) sb.append(r).append('\n');
        Files.writeString(Paths.get(args[1]), sb.toString());
        println("KBEXPORT functions=" + fn.size() + " globals=" + gl.size() + " -> " + args[1]);
    }
}
