import { remoteInfo } from './platform';

export function cloneFolderName(url: string): string {
  const project = remoteInfo(url.trim()).project;
  const leaf = project.split('/').at(-1) ?? '';
  try {
    return decodeURIComponent(leaf).replace(/[^a-z\d_.-]/gi, '-').slice(0, 100);
  } catch {
    return '';
  }
}
