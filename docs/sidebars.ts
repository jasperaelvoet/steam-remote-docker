import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const sidebars: SidebarsConfig = {
  docsSidebar: [
    "introduction",
    {
      type: "category",
      label: "Setup",
      collapsed: false,
      items: [
        "setup/docker",
        "setup/docker-compose",
        "setup/podman-quadlet",
        "setup/first-login",
      ],
    },
    "configuration",
    "idle-lifecycle",
    "operations",
    "data-and-backups",
    "networking",
    "troubleshooting",
    {
      type: "category",
      label: "Reference",
      items: ["reference/cli", "reference/environment"],
    },
    {
      type: "category",
      label: "Internals",
      items: [
        "internals/architecture",
        "internals/streaming",
        "internals/development",
      ],
    },
  ],
};

export default sidebars;
