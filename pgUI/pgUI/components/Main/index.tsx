import React, { FC, ReactNode } from 'react';
import cx from 'classnames';
import styles from './Main.module.scss';
import InstanceNav from '../InstanceNav';

interface Props {
  children: ReactNode;
  hasLeftBar: boolean;
}

const Main: FC<Props> = ({ children, hasLeftBar = false }) => {
  return (
    <div className={cx(styles.wrapper, hasLeftBar && styles.leftBar)}>
      {hasLeftBar && <InstanceNav />}
      <div className={styles.content}>{children}</div>
    </div>
  );
};

export default Main;
