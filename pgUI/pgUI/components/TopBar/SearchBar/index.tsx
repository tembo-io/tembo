import styles from '../TopBar.module.scss';

export default function SearchBar(): JSX.Element {
  return (
    <div className={styles.SearchBar}>
      <input type="search" placeholder="Search CoreDB" />
    </div>
  );
}
