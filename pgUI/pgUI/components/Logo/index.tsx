import React from 'react';
import Image from 'next/image';

import RawLogo from '/public/coredb-logo-globe.png'; // https://www.typescriptlang.org/docs/handbook/module-resolution.html#path-mapping

export default function Logo() {
  return (
    <Image
      src={RawLogo}
      width={32}
      height={32}
      alt="The CoreDB logo - a multicolored globe"
    />
  );
}
