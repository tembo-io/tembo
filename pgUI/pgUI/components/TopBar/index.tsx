import { Avatar } from '@mui/joy';
import cx from 'classnames';

import SearchBar from './SearchBar';
import styles from './TopBar.module.scss';

interface Props {
  shortName: string;
}

export default function TopBar({ shortName }: Props): JSX.Element {
  return (
    <div className={cx(styles.TopBar, 'd-flex v-center')}>
      <SearchBar />
      <div className={cx(styles.filters, 'd-flex v-center')}>
        <svg
          width="20"
          height="20"
          viewBox="0 0 20 20"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <rect
            x="0.399902"
            y="0.279297"
            width="8.51724"
            height="8.71532"
            fill="#C7C2BD"
          />
          <rect
            x="0.399902"
            y="11.0059"
            width="8.51724"
            height="8.71532"
            fill="#C7C2BD"
          />
          <rect
            x="10.8826"
            y="0.279297"
            width="8.51724"
            height="8.71532"
            fill="#C7C2BD"
          />
          <rect
            x="10.8826"
            y="11.0059"
            width="8.51724"
            height="8.71532"
            fill="#C7C2BD"
          />
        </svg>

        <svg
          width="20"
          height="19"
          viewBox="0 0 20 19"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <rect
            x="0.399902"
            y="0.302246"
            width="19"
            height="2.01123"
            fill="#C7C2BD"
          />
          <rect
            x="0.399902"
            y="4.32471"
            width="19"
            height="2.01123"
            fill="#C7C2BD"
          />
          <rect
            x="0.399902"
            y="8.34766"
            width="19"
            height="2.01123"
            fill="#C7C2BD"
          />
          <rect
            x="0.399902"
            y="12.3696"
            width="19"
            height="2.01123"
            fill="#C7C2BD"
          />
          <rect
            x="0.399902"
            y="16.3921"
            width="19"
            height="2.01123"
            fill="#C7C2BD"
          />
        </svg>
      </div>
      <div className="d-flex v-center" style={{ marginLeft: 'auto' }}>
        <button className="btn">Add New</button>
        <div className={cx(styles.UserTag, 'd-flex v-center')}>
          <Avatar size="md" style={{ marginRight: '.5rem' }} />
          <span>{shortName}</span>
        </div>
      </div>
    </div>
  );
}
