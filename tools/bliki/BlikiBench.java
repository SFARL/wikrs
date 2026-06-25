import info.bliki.wiki.model.WikiModel;
import java.nio.file.Files;
import java.nio.file.Paths;

/** Microbenchmark: time the Bliki engine rendering wikitext -> HTML, to compare
 *  against wikrs. Bliki does more work than wikrs's Stage 1 strip (full HTML +
 *  template handling), so this is a reference point, not an apples-to-apples
 *  race. See docs/TESTING.md. */
public class BlikiBench {
    public static void main(String[] args) throws Exception {
        if (args.length < 1) {
            System.err.println("usage: BlikiBench <wikitext-file> [iters]");
            System.exit(2);
        }
        String wikitext = new String(Files.readAllBytes(Paths.get(args[0])), "UTF-8");
        int iters = args.length > 1 ? Integer.parseInt(args[1]) : 2000;

        String html = render(wikitext);
        System.err.printf("ok: %d bytes wikitext -> %d bytes HTML%n",
                wikitext.getBytes("UTF-8").length, html.length());

        for (int i = 0; i < Math.min(300, iters); i++) render(wikitext); // warmup

        long bytes = wikitext.getBytes("UTF-8").length;
        long start = System.nanoTime();
        long sink = 0;
        for (int i = 0; i < iters; i++) sink += render(wikitext).length();
        double secs = (System.nanoTime() - start) / 1e9;
        double mbps = (bytes * (double) iters) / 1e6 / secs;
        System.out.printf("bliki  %.3f s  %.1f MB/s  (%d iters, sink=%d)%n", secs, mbps, iters, sink);
    }

    static String render(String wikitext) throws java.io.IOException {
        WikiModel model = new WikiModel("https://example.org/wiki/${image}",
                                        "https://example.org/wiki/${title}");
        return model.render(wikitext);
    }
}
