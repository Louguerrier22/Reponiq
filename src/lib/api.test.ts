import { describe, expect, it } from 'vitest';
import { filteredRepos, type Repo } from './api';
const base: Repo = { id: 1, name: 'core', path: '/core', remote_url: null, pinned: false, collection: 'Work', tags: ['rust'], branch: 'main', dirty: 0, ahead: 0, behind: 0, github_full_name: null, ci_conclusion: null, ci_status: null };
describe('repository filters', () => {
  it('searches tags and collections case-insensitively', () => expect(filteredRepos([base], 'RUST', 'all')).toHaveLength(1));
  it('filters attention and pins separately', () => { const pinned = {...base, id:2, pinned:true, behind:1}; expect(filteredRepos([base,pinned], '', 'attention').map(r => r.id)).toEqual([2]); expect(filteredRepos([base,pinned], '', 'pinned')).toHaveLength(1); });
  it('places pinned repos first', () => expect(filteredRepos([base, {...base,id:3,name:'z',pinned:true}], '', 'all')[0].id).toBe(3));
});

describe('release regressions',()=>{
 it('filters collections before counting and pagination',()=>{expect(filteredRepos([base,{...base,id:2,collection:'Personal'}],'','collection:Work').map(r=>r.id)).toEqual([1]);});
 it('includes unavailable repositories in attention',()=>{expect(filteredRepos([{...base,status_error:'Missing folder'}],'','attention')).toHaveLength(1);});
 it('keeps the source collection order unchanged',()=>{const rows=[{...base,name:'z'}, {...base,id:2,name:'a'}];filteredRepos(rows,'','all');expect(rows[0].name).toBe('z');});
});
