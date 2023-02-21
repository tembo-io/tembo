import React, { FC, useMemo } from 'react';
import cx from 'classnames';
import { useRouter } from 'next/router';
import Link from 'next/link';

import Card from '../Card';
import Chip from '../Chip';

import styles from './InstanceCard.module.scss';

interface Props {
  dbName: string;
  id: string;
  paths: any;
  hasMenuOptions?: boolean;
  properties: {
    connection: string;
    cpu: number;
    dbname: string;
    environment: 'test' | 'dev' | 'prod';
    extensions: Array<string>;
    memory: string;
    size: number;
    status:
      | 'Submitted'
      | 'Provisioning'
      | 'Up'
      | 'Deleted'
      | 'Suspended'
      | 'Restarting';
  };
}

const InstanceCard: FC<Props> = ({
  properties,
  id,
  dbName,
  paths,
  hasMenuOptions,
}) => {
  const router = useRouter();
  // Map status to colors for chip
  const statusColor = useMemo(() => {
    switch (properties?.status) {
      case 'Up':
        return 'success';
      case 'Deleted':
        return 'error';
      case 'Suspended':
        return 'primary-5';
      case 'Submitted':
        return 'primary-9';
      case 'Restarting':
        return 'accent';
      case 'Provisioning':
        return 'accent-darker';
      default:
        return 'accent';
    }
  }, [properties?.status]);
  return (
    <Link
      //   href={
      //     paths.entities({ ...router.query, entity: dbName, id: String(id) }).view
      //   }>
      href="/">
      <Card>
        {hasMenuOptions && <button className={styles.contextMenu}>more</button>}
        <Chip label={properties.status} type={statusColor} />
        <h3 className={cx(styles.cardTitle, 'type-medium')}>
          {properties?.dbname}
        </h3>
        <h4 className={cx(styles.cardSubtitle, 'type-medium')}>
          {properties.environment}
        </h4>
        <div className={cx(styles.stats, 'd-flex v-center')}>
          <div className={styles.stat}>
            <span>Memory</span>
            <span>{properties.memory ?? 'N/A'}</span>
          </div>
          <div className={styles.stat}>
            <span>CPU</span>
            <span>{properties.cpu ?? 'NA'}</span>
          </div>
          <div className={styles.stat}>
            <span>Extensions</span>
            <span>{properties?.extensions?.length ?? 0}</span>
          </div>
          {/* <div className={styles.stat}>
            <span>Env</span>
            <span>{properties?.environment}</span>
          </div> */}
        </div>
      </Card>
    </Link>
  );
};

export default InstanceCard;
