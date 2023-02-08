import { useColorScheme } from '@mui/joy/styles';
import Button from '@mui/joy/Button';
// import LightModeIcon from '@mui/icons-material/LightMode';
// import DarkModeIcon from '@mui/icons-material/DarkMode';

export default function ModeToggle() {
  const { mode, setMode } = useColorScheme();
  return (
    <Button
      variant="plain"
      color="neutral"
      onClick={() => setMode(mode === 'dark' ? 'light' : 'dark')}
    >
      {/* {mode === 'dark' ? <LightModeIcon /> : <DarkModeIcon />} */}
    </Button>
  );
}
