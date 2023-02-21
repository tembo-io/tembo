import React, { FC } from 'react';
import cx from 'classnames';
import Link from 'next/link';

import Logo from '../Logo';

import styles from './Header.module.scss';

interface HeaderProps {
  shortName: string;
}

const Header: FC<HeaderProps> = ({ shortName }) => {
  return (
    <header className={cx(styles.Header, 'd-flex v-center')}>
      <span className={cx(styles.logoGroup, 'd-flex v-center')}>
        <Logo />
        <h1>CoreDB</h1>
      </span>
      <h4>Organization ▼</h4>
      <div className="d-flex" style={{ marginLeft: 'auto' }}>
        <nav>
          <ul className="d-flex v-center">
            <Link href="/">
              <li>Instances</li>
            </Link>
            <Link href="/team">
              <li>Team</li>
            </Link>
            <Link href="/billing">
              <li>Billing</li>
            </Link>
            <Link href="/settings">
              <li>Settings</li>
            </Link>
            <Link href="/help">
              <li>Help</li>
            </Link>
          </ul>
        </nav>
        <div className={cx(styles.UserTag, 'd-flex v-center')}>
          <div className={styles.avatar} />
          <span>{shortName}</span>
        </div>
      </div>
    </header>
  );
};

export default Header;
