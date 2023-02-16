import { CircularProgress, Stack } from '@mui/joy';

import TopBar from '../TopBar';
import LeftNavigation from '../LeftNav';

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
        {<TopBar shortName="jon" />}
        {children}
      </div>
    </Stack>
  );
}
