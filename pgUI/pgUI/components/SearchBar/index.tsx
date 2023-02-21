import React, { FC } from 'react';
import styles from './SearchBar.module.scss';

interface SearchProps {
  placeholder?: string;
}

const SearchBar: FC<SearchProps> = ({ placeholder = 'Search Instances' }) => {
  return (
    <div className={styles.SearchBar}>
      <input type="search" placeholder={placeholder} />
    </div>
  );
};

export default SearchBar;
