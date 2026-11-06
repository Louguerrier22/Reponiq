import {describe,it,expect} from 'vitest';
import {remoteInfo} from './platform';
describe('Git platform display detection',()=>{
  it.each([
    ['git@github.com:owner/repo.git','github','owner/repo'],
    ['https://gitlab.com/team/subgroup/repo.git','gitlab','team/subgroup/repo'],
    ['ssh://git@gitlab.example.com:2222/team/repo.git','gitlab','team/repo'],
    ['git@bitbucket.org:team/repo.git','bitbucket','team/repo'],
    ['https://dev.azure.com/team/project/_git/repo','azure','team/project/_git/repo'],
    ['git@ssh.dev.azure.com:v3/team/project/repo','azure','v3/team/project/repo'],
    ['https://codeberg.org/owner/repo.git','codeberg','owner/repo'],
    ['ssh://git@gitea.example.org/owner/repo.git','gitea','owner/repo'],
    ['https://forgejo.example.org/owner/repo.git','forgejo','owner/repo'],
    ['git@git.sr.ht:~user/repo','sourcehut','~user/repo'],
  ])('recognizes %s',(remote,platform,project)=>expect(remoteInfo(remote)).toMatchObject({platform,project}));
  it('does not mislabel lookalike domains or local paths',()=>{
    for(const remote of ['https://github.com.evil.test/o/r','C:\\projects\\repo','/home/user/repo','file:///tmp/repo','invalid'])expect(remoteInfo(remote).platform).toBe('git');
    expect(remoteInfo(null).platform).toBe('local');
  });
  it('never exposes URL credentials, query strings or fragments in the project label',()=>{
    expect(remoteInfo('https://user:secret@gitlab.com/team/repo.git?token=secret#secret')).toMatchObject({host:'gitlab.com',project:'team/repo'});
  });
});
