interface Option {
  label: string;
  link: string;
  iconName:
    | 'activity'
    | 'codesandbox'
    | 'compass'
    | 'database'
    | 'server'
    | 'sliders'
    | 'terminal'
    | 'users';
}

const navOptions: Array<Option> = [
  { label: 'Overview', link: '/overview˝', iconName: 'server' },
  { label: 'SQL Runner', link: '/sql-runner', iconName: 'terminal' },
  { label: 'Object Explorer', link: '/object-explorer', iconName: 'compass' },
  { label: 'Monitoring', link: '/monitoring', iconName: 'activity' },
  { label: 'Users', link: '/users', iconName: 'users' },
  { label: 'Configuration', link: '/configuration', iconName: 'sliders' },
  { label: 'Extensions', link: '/extensions', iconName: 'codesandbox' },
];

export default navOptions;
