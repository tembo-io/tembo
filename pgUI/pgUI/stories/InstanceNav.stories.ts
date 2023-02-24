import type { Meta, StoryObj } from '@storybook/react';

import InstanceNav from '../components/InstanceNav';

// More on how to set up stories at: https://storybook.js.org/docs/7.0/react/writing-stories/introduction
const meta = {
  title: 'Example/InstanceNav',
  component: InstanceNav,
  tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof InstanceNav>;

export default meta;
type Story = StoryObj<typeof meta>;

// More on writing stories with args: https://storybook.js.org/docs/7.0/react/writing-stories/args
export const Basic: Story = {
  args: {},
};
