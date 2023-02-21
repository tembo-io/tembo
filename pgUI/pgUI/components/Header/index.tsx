import React, { FC } from 'react';
import cx from 'classnames';
import Link from 'next/link';

import Logo from '../Logo';

import styles from './Header.module.scss';
import Button from '../Button';

interface HeaderProps {
  userName: string;
}

const Header: FC<HeaderProps> = ({ userName }) => {
  return (
    <header className={cx(styles.Header, 'd-flex v-center')}>
      <span className={cx(styles.logoGroup, 'd-flex v-center')}>
        <Logo />
        <h1>CoreDB</h1>
      </span>
      <h4>Organization</h4>
      <div className="d-flex" style={{ marginLeft: 'auto' }}>
        <nav>
          <ul className="d-flex v-center">
            <Link href="/">
              <li>Instances</li>
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
          <span>{userName}</span>
        </div>
      </div>
    </header>
  );
};

export default Header;
