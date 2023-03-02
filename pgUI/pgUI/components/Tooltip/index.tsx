import React, { FC, ReactElement } from 'react';
import * as RadixToolTip from '@radix-ui/react-tooltip';
import styles from './Tooltip.module.scss';

interface Props {
  text: string;
  children: ReactElement;
}

const Tooltip: FC<Props> = ({ children, text }) => {
  return (
    <RadixToolTip.Provider delayDuration={0}>
      <RadixToolTip.Root>
        <RadixToolTip.Trigger asChild>{children}</RadixToolTip.Trigger>
        <RadixToolTip.Portal>
          <RadixToolTip.Content
            side="right"
            className={styles.TooltipContent}
            sideOffset={5}
          >
            {text}
            <RadixToolTip.Arrow className={styles.TooltipArrow} />
          </RadixToolTip.Content>
        </RadixToolTip.Portal>
      </RadixToolTip.Root>
    </RadixToolTip.Provider>
  );
};

export default Tooltip;
