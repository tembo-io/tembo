import React, { FC } from 'react';

import styles from './Button.module.scss';

interface ButtonProps {
  children: React.ReactNode;
  onClick?(): any;
}

const Button: FC<ButtonProps> = ({ children, onClick }) => {
  return (
    <button onClick={onClick} className={styles.btn}>
      {children}
    </button>
  );
};

export default Button;
