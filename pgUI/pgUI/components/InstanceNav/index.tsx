import React, { FC, useState } from 'react';
import { useRouter } from 'next/router';
import Link from 'next/link';

import IconButton from '../IconButton';
import Tooltip from '../Tooltip';

import styles from './InstanceNav.module.scss';
import iconButtonStyles from '../Button/Button.module.scss';
import navOptions from './instanceNavData';

const InstanceNav: FC = () => {
  const router = useRouter();
  console.log('router', router.asPath);
  const pathToId = router.asPath.match(/.+?(\d)/);
  const instancePath = pathToId !== null ? pathToId[0] : '';
  return (
    <div className={styles.instanceNav}>
      {navOptions.map((option) => (
        <Link
          href={instancePath + option.link}
          key={option.label}
          className={styles.link}>
          <Tooltip text={option.label}>
            <IconButton
              iconName={option.iconName}
              className={
                router.asPath.toLowerCase().includes(option.link) &&
                iconButtonStyles.active
              }
            />
          </Tooltip>
        </Link>
      ))}
    </div>
  );
};

export default InstanceNav;
