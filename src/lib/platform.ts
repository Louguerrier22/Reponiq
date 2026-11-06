export type Platform = 'github' | 'gitlab' | 'bitbucket' | 'azure' | 'codeberg' | 'gitea' | 'forgejo' | 'sourcehut' | 'git' | 'local';
export type RemoteInfo = { platform: Platform; label: string; host: string; project: string };
const labels: Record<Platform, string> = { github:'GitHub',gitlab:'GitLab',bitbucket:'Bitbucket',azure:'Azure DevOps',codeberg:'Codeberg',gitea:'Gitea',forgejo:'Forgejo',sourcehut:'SourceHut',git:'Git',local:'Local repository' };

// Display hints only. Host detection never selects an API endpoint or credential.
export function remoteInfo(remote: string | null): RemoteInfo {
  let host='',project='';
  if(remote && remote.length<=32768){
    try {
      if(/^[a-z][a-z\d+.-]*:\/\//i.test(remote)){
        const url=new URL(remote);
        if(['https:','http:','ssh:','git:'].includes(url.protocol)){host=url.hostname.toLowerCase();project=url.pathname.replace(/^\/+|\/+$/g,'');}
      } else {
        const match=remote.match(/^(?:[^@/:\s]+@)?([a-z\d.-]+):(.+)$/i);
        if(match && !/^[a-z]$/i.test(match[1])){host=match[1].toLowerCase();project=match[2].replace(/^\/+|\/+$/g,'').split(/[?#]/,1)[0];}
      }
    }catch{/* An unsupported/local remote stays generic. */}
  }
  project=project.replace(/\.git$/i,'');
  let platform: Platform=remote?'git':'local';
  const prefix=host.split('.')[0];
  if(host==='github.com'||host==='ssh.github.com'||prefix==='github'&&!host.startsWith('github.com.'))platform='github';
  else if(host==='gitlab.com'||prefix==='gitlab'&&!host.startsWith('gitlab.com.'))platform='gitlab';
  else if(host==='bitbucket.org'||prefix==='bitbucket'&&!host.startsWith('bitbucket.org.'))platform='bitbucket';
  else if(host==='dev.azure.com'||host==='ssh.dev.azure.com'||host.endsWith('.visualstudio.com'))platform='azure';
  else if(host==='codeberg.org')platform='codeberg';
  else if(host==='gitea.com'||prefix==='gitea')platform='gitea';
  else if(prefix==='forgejo')platform='forgejo';
  else if(host==='git.sr.ht'||host==='sr.ht')platform='sourcehut';
  return {platform,label:labels[platform],host,project};
}
