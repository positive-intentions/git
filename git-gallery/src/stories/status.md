# Status

Shows `GitRepo::status` after simple mutations.

1. **Init** a throwaway repo  
2. **Write** a sample file (staged)  
3. **Edit** without relying on knobs (appends a marker)  
4. **Status** to list working-tree / index changes  
5. **Remove** then **Status** again  

On an unborn `HEAD`, staged paths are reported as `staged`.
