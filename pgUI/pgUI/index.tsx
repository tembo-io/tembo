import { Stack } from '@mui/joy';

import LeftNavigation from './components/LeftNav';
import TopBar from './components/TopBar';

import styles from './Main.module.scss';

export default function Main(
  props: React.PropsWithChildren & {
    hasRightSidebar?: boolean;
    allowCreation?: boolean;
    entity?: string;
  }
) {
  const { children, allowCreation, entity } = props;

  return (
    <Stack
      direction="row"
      sx={{
        width: '100%',
      }}>
      <LeftNavigation allowCreation={allowCreation} entity={entity} />
      <div className={styles.DashboardContainer}>
        {<TopBar shortName={'jon'} />}
        {children}
      </div>
    </Stack>
  );
}
