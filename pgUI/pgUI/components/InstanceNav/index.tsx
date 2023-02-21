import { FC } from 'react';
import { useRouter } from 'next/router';
import Link from 'next/link';

import styles from './InstanceNav.module.scss';
import navOptions from './instanceNavData';

const InstanceNav: FC = () => {
  const router = useRouter();
  return (
    <div className={styles.LeftNavigation}>
      {navOptions.map((option, index) => (
        <Link href={option.link} key={index}>
          <h5>{option.label}</h5>
        </Link>
      ))}
    </div>
  );
};

export default InstanceNav;
