import type { Meta, StoryObj } from '@storybook/react';

import IconButton from '../components/IconButton';

// More on how to set up stories at: https://storybook.js.org/docs/7.0/react/writing-stories/introduction
const meta = {
  title: 'Example/IconButton',
  component: IconButton,
  tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof IconButton>;

export default meta;
type Story = StoryObj<typeof meta>;

// More on writing stories with args: https://storybook.js.org/docs/7.0/react/writing-stories/args
export const Basic: Story = {
  args: {
    iconName: 'codesandbox',
    onClick: () => alert('you clicked the button')
  },
};
