import { FC, ReactNode } from 'react';
import styles from './Main.module.scss';

interface Props {
  children: ReactNode;
}

const Main: FC<Props> = ({ children }) => {
  return <div className={styles.wrapper}>{children}</div>;
};

export default Main;
