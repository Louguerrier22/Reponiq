import { describe, expect, it } from 'vitest';
import { cloneFolderName } from './clone';

describe('clone folder suggestions', () => {
  it.each([
    ['https://github.com/owner/project.git', 'project'],
    ['https://gitlab.com/team/subgroup/project/', 'project'],
    ['git@github.com:owner/project.git', 'project'],
    ['ssh://git@example.com:2222/team/project.git', 'project'],
    [' https://example.com/team/my%20project.git ', 'my-project'],
    ['', ''],
    ['not a URL', ''],
  ])('suggests a folder for %s', (url, name) => {
    expect(cloneFolderName(url)).toBe(name);
  });
});
