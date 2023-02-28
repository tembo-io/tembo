import React, { FC } from 'react';
import { useRouter } from 'next/router';
import Link from 'next/link';

import IconButton from '../IconButton';
import Tooltip from '../Tooltip';

import styles from './InstanceNav.module.scss';
import navOptions from './instanceNavData';

const InstanceNav: FC = () => {
  const router = useRouter();
  console.log(router);
  return (
    <div className={styles.instanceNav}>
      {navOptions.map((option) => (
        <Link
          href={router.asPath + option.link}
          key={option.label}
          className={styles.link}
        >
          <Tooltip text={option.label}>
            <IconButton iconName={option.iconName} />
          </Tooltip>
        </Link>
      ))}
    </div>
  );
};

export default InstanceNav;
