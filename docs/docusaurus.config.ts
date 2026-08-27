import type * as Preset from "@docusaurus/preset-classic";
import type { Config } from "@docusaurus/types";
import { themes as prismThemes } from "prism-react-renderer";

const config: Config = {
  title: "steam-remote-docker",
  tagline: "A headless Steam Remote Play host in a container",

  url: "https://jasperaelvoet.github.io",
  baseUrl: "/steam-remote-docker/",

  organizationName: "jasperaelvoet",
  projectName: "steam-remote-docker",
  trailingSlash: false,

  favicon: "img/logo.svg",

  onBrokenLinks: "throw",

  markdown: {
    hooks: {
      onBrokenMarkdownLinks: "warn",
    },
  },

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  presets: [
    [
      "classic",
      {
        docs: {
          routeBasePath: "/",
          sidebarPath: "./sidebars.ts",
          editUrl:
            "https://github.com/jasperaelvoet/steam-remote-docker/tree/main/docs/",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],

  themes: [
    [
      "@easyops-cn/docusaurus-search-local",
      {
        hashed: true,
        indexBlog: false,
        docsRouteBasePath: "/",
      },
    ],
  ],

  themeConfig: {
    colorMode: {
      defaultMode: "dark",
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: "steam-remote-docker",
      logo: {
        alt: "steam-remote-docker logo",
        src: "img/logo.svg",
      },
      items: [
        {
          type: "docSidebar",
          sidebarId: "docsSidebar",
          position: "left",
          label: "Docs",
        },
        {
          href: "https://github.com/jasperaelvoet/steam-remote-docker/pkgs/container/steam-remote-docker",
          label: "Image",
          position: "right",
        },
        {
          href: "https://github.com/jasperaelvoet/steam-remote-docker",
          label: "GitHub",
          position: "right",
        },
      ],
    },
    footer: {
      style: "dark",
      links: [
        {
          title: "Docs",
          items: [
            { label: "Introduction", to: "/" },
            { label: "Setup", to: "/setup/docker" },
            { label: "Troubleshooting", to: "/troubleshooting" },
          ],
        },
        {
          title: "More",
          items: [
            {
              label: "GitHub",
              href: "https://github.com/jasperaelvoet/steam-remote-docker",
            },
            {
              label: "Container image",
              href: "https://github.com/jasperaelvoet/steam-remote-docker/pkgs/container/steam-remote-docker",
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Jasper Aelvoet. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ["bash", "json", "ini", "yaml"],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
