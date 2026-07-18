import { defineConfig, markdown } from "sourcey";

const canonicalUrl = new URL("https://tokio-rs.github.io/bytes/field-guide/");

export default defineConfig({
  name: "Bytes Field Guide",
  siteUrl: canonicalUrl.origin,
  baseUrl: canonicalUrl.pathname,
  repo: "https://github.com/tokio-rs/bytes",
  editBranch: "master",
  editBasePath: "docs/sourcey",
  navigation: {
    tabs: [
      {
        tab: "Guide",
        slug: "",
        source: markdown({
          groups: [
            { group: "Start", pages: ["index", "installation"] },
            { group: "Buffer Types", pages: ["bytes", "bytes-mut"] },
            { group: "Traits", pages: ["buf", "buf-mut"] },
            { group: "Workflows", pages: ["adapters", "patterns"] },
          ],
        }),
      },
    ],
  },
});
