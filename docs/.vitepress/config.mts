import { defineConfig } from 'vitepress';

export default defineConfig({
  title: 'Steam Remote Play container',
  description:
    'An opinionated, headless Steam host for Steam Link: one Gamescope session, PipeWire audio, and AMD hardware acceleration in a read-only OCI image.',
  base: '/steam-remote-docker/',
  lang: 'en-US',
  lastUpdated: true,
  cleanUrls: true,

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/steam-remote-docker/logo.svg' }],
    ['meta', { property: 'og:title', content: 'Steam Remote Play container' }],
    [
      'meta',
      {
        property: 'og:description',
        content: 'An opinionated, headless Steam host for Steam Link.',
      },
    ],
  ],

  themeConfig: {
    logo: '/logo.svg',

    nav: [
      { text: 'Guide', link: '/guide/getting-started', activeMatch: '/guide/' },
      { text: 'Reference', link: '/reference/cli', activeMatch: '/reference/' },
      { text: 'Internals', link: '/internals/architecture', activeMatch: '/internals/' },
      {
        text: 'Image',
        link: 'https://github.com/jasperaelvoet/steam-remote-docker/pkgs/container/steam-remote-docker',
      },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [
            { text: 'Getting started', link: '/guide/getting-started' },
            { text: 'Configuration', link: '/guide/configuration' },
            { text: 'Idle lifecycle', link: '/guide/idle-lifecycle' },
            { text: 'Operations', link: '/guide/operations' },
            { text: 'Persistent data & backups', link: '/guide/persistent-data' },
            { text: 'Troubleshooting', link: '/guide/troubleshooting' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: [
            { text: 'steam-remote CLI', link: '/reference/cli' },
            { text: 'Environment variables', link: '/reference/environment' },
            { text: 'Networking & ports', link: '/reference/networking' },
          ],
        },
      ],
      '/internals/': [
        {
          text: 'Internals',
          items: [
            { text: 'Architecture', link: '/internals/architecture' },
            { text: 'Streaming pipeline', link: '/internals/streaming' },
            { text: 'Development', link: '/internals/development' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/jasperaelvoet/steam-remote-docker' },
    ],

    search: {
      provider: 'local',
    },

    editLink: {
      pattern:
        'https://github.com/jasperaelvoet/steam-remote-docker/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },

    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © Jasper Aelvoet',
    },

    outline: { level: [2, 3] },
  },
});
