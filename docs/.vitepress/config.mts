import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'superdupermemory',
  description: 'Local-first memory layer for AI agents',
  base: '/superdupermemory/',
  ignoreDeadLinks: true,

  head: [['link', { rel: 'icon', href: '/superdupermemory/favicon.ico' }]],

  themeConfig: {
    nav: [
      { text: 'Guide', link: '/guide/' },
      { text: 'API', link: '/api/' },
      { text: 'SDK', link: '/sdk/' },
      { text: 'GitHub', link: 'https://github.com/avirajkhare00/superdupermemory' },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'What is superdupermemory?', link: '/guide/' },
            { text: 'Quick Start', link: '/guide/quickstart' },
            { text: 'Self-hosting', link: '/guide/self-hosting' },
            { text: 'Configuration', link: '/guide/configuration' },
          ],
        },
      ],
      '/api/': [
        {
          text: 'REST API',
          items: [
            { text: 'Overview', link: '/api/' },
            { text: 'Orgs & Apps', link: '/api/orgs' },
            { text: 'Memories', link: '/api/memories' },
          ],
        },
      ],
      '/sdk/': [
        {
          text: 'TypeScript SDK',
          items: [
            { text: 'Installation', link: '/sdk/' },
            { text: 'SupduperMemory', link: '/sdk/memory' },
            { text: 'SupduperMemoryAdmin', link: '/sdk/admin' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/avirajkhare00/superdupermemory' },
    ],

    footer: {
      message: 'Apache 2.0 License',
    },
  },
})
