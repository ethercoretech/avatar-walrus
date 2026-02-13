// SPDX-License-Identifier: MIT
pragma solidity ^0.8.31;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract MiniUSDT is ERC20, Ownable {
    uint8 private constant _decimals = 6;
    
    constructor() ERC20("Mini USDT", "USDT") Ownable(msg.sender) {
        _mint(msg.sender, 1000000 * 10 ** _decimals);
    }
    
    function decimals() public pure override returns (uint8) {
        return _decimals;
    }
    
    function mint(address to, uint256 amount) public onlyOwner {
        _mint(to, amount);
    }
    
    function burn(uint256 amount) public {
        _burn(msg.sender, amount);
    }
}
