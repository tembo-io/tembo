interface Option {
  label: string;
  link: string;
}

const navOptions: Array<Option> = [
  { label: 'Overview', link: '/overview˝' },
  { label: 'SQL Runner', link: '/sql-runner' },
  { label: 'Object Explorer', link: '/object-explorer' },
  { label: 'Monitoring', link: '/monitoring' },
  { label: 'Users', link: '/users' },
  { label: 'Configuration', link: '/configuration' },
  { label: 'Extensions', link: '/extensions' },
];

export default navOptions;
