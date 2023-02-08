import { useRouter } from 'next/router';
import Link from 'next/link';

import styles from './LeftNav.module.scss';


type Props = {
  allowCreation?: boolean;
  entity?: string;
};
export default function LeftNavigation(props: Props) {
  const router = useRouter();
  const { allowCreation = false, entity } = props;

  return (
    <div className={styles.LeftNavigation}>
      <section className={styles.logoSection}>
        <h1>CoreDB</h1>
      </section>
      <section>
        <h4 className={styles.leftNavTitle}>Databases</h4>
        <div className={styles.orgList}>

        </div>
        <button
          className="btn small secondary"
          onClick={() => alert('choose a new org')}
        >
          + Database
        </button>

      </section>
      <section className={styles.helpSection}>
        <Link className="type-small" href="/">
          Support
        </Link>
        <Link className="type-small" href="/">
          Docs
        </Link>
        <Link className="type-small" href="/">
          API
        </Link>
      </section>
    </div>
  );
}
