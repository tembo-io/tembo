import React, { FC } from 'react';

import styles from './Chip.module.scss';

interface Props {
  label: string;
  // This could be improved with specific types but for now
  // they represent color vars
  type:
    | 'accent'
    | 'success'
    | 'error'
    | 'primary-5'
    | 'primary-9'
    | 'accent-darker';
}

const Chip: FC<Props> = ({ label, type }) => {
  return (
    <div className={styles.chip}>
      <div
        className={styles.colorSwatch}
        style={{ background: `var(--${type})` }}
      />
      <span>{label}</span>
    </div>
  );
};

export default Chip;
