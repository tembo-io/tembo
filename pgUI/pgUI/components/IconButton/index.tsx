import React, { FC } from 'react';
import cx from 'classnames';
import iconList from '../../public/icons/iconList';

import styles from '../Button/Button.module.scss';

interface IconButtonProps {
  iconName:
    | 'activity'
    | 'codesandbox'
    | 'compass'
    | 'database'
    | 'server'
    | 'sliders'
    | 'terminal'
    | 'users';
  onClick?(): any;
}

const IconButton: FC<IconButtonProps> = ({
  iconName = 'activity',
  onClick,
}) => {
  const selectedIcon = iconList[iconName].src;
  return (
    <button onClick={onClick} className={cx(styles.btn, styles.iconBtn)}>
      {/* <img src={`/icons/${iconName}.svg`} alt={iconList[iconName].alt} /> */}
      {selectedIcon}
    </button>
  );
};

export default IconButton;
